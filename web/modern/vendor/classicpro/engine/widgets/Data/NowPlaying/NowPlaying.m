/*
SC:NowPlaying XUI Object
also incorporates nested SC:FadeText

pjn123, Quadhelix, SLoB

2008
this is a wip, please do not rip it without giving proper credit
*/

#include <lib/std.mi>
#include <lib/colormgr.mi>
#include "../../../lib/ClassicProFile.mi"

Function refreshView();
Function setAllTags();
Function resizeToThis(int x, int y, int w, int h);
Function FadeGroup(Group FadeGrp, Int AlphaValue);
Function refreshCover();
Function String getMyFile();

Global Group XUIGroup, cdbox, cdboxref, cdboxHolder, ratings, cdbox_2;
Global GuiObject line1, line2, line3, BGCol, CDBoxFade;
Global Timer delayMyResize;
Global int xs, ys, ws, hs;
Global int xc, yc, wc, hc;
Global layer lyrFx, lyrFxFG, mousetrap_bottom, cdbox1, cdbox2, cdbox3; //, CDBoxFade2;
global double dblSmidge;
Global int reflectionheight;
Global AlbumArtLayer albumart, AlbumArt2;
Global PopUpMenu popMenu;
Global Timer lookagain;

System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	ratings = XUIGroup.findObject("sc.nowplaying.ratings");
	
	cdbox = XUIGroup.findObject("sc.nowplaying.cdbox.holder.top");
	cdbox_2 = XUIGroup.findObject("sc.nowplaying.cdbox.holder.bottom");
	albumart = XUIGroup.findObject("main.albumart");
	AlbumArt2 = XUIGroup.findObject("main.albumart.reflection");
	
	cdboxref = XUIGroup.findObject("sc.nowplaying.cdbox.reflection"); 
	
	cdboxHolder = XUIGroup.findObject("sc.nowplaying.cdbox.holder");
	lyrFx = XUIGroup.findObject("main.albumart.reflection");
	lyrFxFG = XUIGroup.findObject("cdbox.fg.reflection");
	line1 = XUIGroup.findObject("sc.nowplaying.line1");
	line2 = XUIGroup.findObject("sc.nowplaying.line2");
	line3 = XUIGroup.findObject("sc.nowplaying.line3");
	
	mousetrap_bottom = XUIGroup.findObject("sc.nowplaying.mousetrap.bottom");
	cdbox1 = XUIGroup.findObject("sc.cdbox.1");
	cdbox2 = XUIGroup.findObject("sc.cdbox.2");
	cdbox3 = XUIGroup.findObject("sc.cdbox.3");
	 

	lookagain = new Timer;
	lookagain.setDelay(1000);
	
	//BGCol = XUIGroup.findObject("sc.nowplaying.bg");
	CDBoxFade = XUIGroup.findObject("cdbox.fg.fademask");
	//CDBoxFade2 = XUIGroup.findObject("cdbox.fg.fademask2");

	/*if (!CDBoxFade2.isInvalid())
	{
		CDBoxFade.hide();
		CDBoxFade2.show();
	}*/
	
	// Reader for albumart gradient (remove later... just keep here to see what code was used)... this will be done inside the widget from v1.1
	//Map myMap = new Map;
	//myMap.loadMap("wasabi.list.background");
	Color myColor = ColorMgr.getColor("wasabi.list.background");
	//XUIGroup.setXmlParam("bgcolor", integerToString(myMap.getARGBValue(0,0,2))+","+integerToString(myMap.getARGBValue(0,0,1))+","+integerToString(myMap.getARGBValue(0,0,0)));
	String temp = integerToString(myColor.getRed())+","+integerToString(myColor.getGreen())+","+integerToString(myColor.getBlue());
	//XUIGroup.setXmlParam("bgcolor", temp);
	System.onSetXuiParam("bgcolor", temp);
	delete myColor;
	//delete myMap;
		
	delayMyResize = new Timer;
	delayMyResize.setDelay(100);
	
	lyrFx.fx_setBgFx(0);
  	lyrFx.fx_setWrap(0);
  	lyrFx.fx_setBilinear(1);
	lyrFx.fx_setAlphaMode(0);
  	lyrFx.fx_setGridSize(1,1);
  	lyrFx.fx_setRect(1);
	lyrFx.fx_setClear(0); //left as zero as cover is a static image and we dont need to redraw
  	lyrFx.fx_setLocalized(1);
  	lyrFx.fx_setRealtime(1);
  	lyrFx.fx_setSpeed(2000); //slow timer as we dont really need to redraw much
	
	lyrFxFG.fx_setBgFx(0);
  	lyrFxFG.fx_setWrap(0);
  	lyrFxFG.fx_setBilinear(1);
	lyrFxFG.fx_setAlphaMode(0);
  	lyrFxFG.fx_setGridSize(1,1);
  	lyrFxFG.fx_setRect(1);
	lyrFxFG.fx_setClear(0); //left as zero as cover sheen is a static image and we dont need to redraw
  	lyrFxFG.fx_setLocalized(1);
  	lyrFxFG.fx_setRealtime(1);
  	lyrFxFG.fx_setSpeed(2000); //slow timer as we dont really need to redraw much
	
  	dblSmidge = 0;
	//default half height - remove ability to resize for the now
	reflectionheight=50;
	//cdboxHolder.Resize(0, 0, 190, 190);
	
		
}
/*
System.onKeyDown(String key){
	// lyrFx.fx_setSpeed(1); //slow timer as we dont really need to redraw much
  	//lyrFxFG.fx_setSpeed(1); //slow timer as we dont really need to redraw much

	//debug(key);
	
	if(key=="") return;
	
	lyrFx.fx_setEnabled(0);
	lyrFxFG.fx_setEnabled(0);
	lyrFx.fx_setEnabled(1);
	lyrFxFG.fx_setEnabled(1);
	lyrFx.fx_restart();
	lyrFxFG.fx_restart();

}*/

AlbumArt2.onAlbumArtLoaded(boolean success){
	lyrFx.fx_setEnabled(0);
	lyrFxFG.fx_setEnabled(0);
	lyrFx.fx_setEnabled(1);
	lyrFxFG.fx_setEnabled(1);
	lyrFx.fx_restart();
	lyrFxFG.fx_restart();
}

system.onScriptUnloading()
{
	XUIGroup = NULL;
	delete delayMyResize;
	delete lookagain;
}

System.onSetXuiParam(String param, String value)
{
	if(strlower(param) == "bgcolor")
	{
		string sbgcol = strlower(value);
		//BGCol.setXMLParam("color", sbgcol);

		//gradient fade to mask the reflection from image to background ie fade it to nothing
		CDBoxFade.setXMLParam("gammagroup", CDBoxFade.getXMLParam("gammagroup"));
		CDBoxFade.setXMLParam("points", "0.0=" + sbgcol + ",50;1.0=" + sbgcol + ",255"); //pjn mix
		
	}
	else if(strlower(param) == "reflectionheightpercentage")
	{
		//default to 50
		if(stringtointeger(value)>100 || stringtointeger(value)<0 || value=="")
		{
			reflectionheight = 50;
		}
		else
		{
			reflectionheight = stringtointeger(value);
		}
	}
	
}

FadeGroup(Group FadeGrp, Int AlphaValue)
{	
	FadeGrp.setTargetA(AlphaValue);
  	FadeGrp.setTargetSpeed(1.3);
  	FadeGrp.gotoTarget();
	complete;	
}

lyrFx.fx_onFrame()
{
	if(dblSmidge == 0.000001) 
	{
		dblSmidge = 0;
	}
	else
	{
		dblSmidge = 0.000001;
	}

}

//reflection, needs dblSmidge to activate, its on a slow timer without clear as its a static image and doesnt need to be cleared every frame
//Wow.. what a mouthful.. (that's what she said)
lyrFx.fx_onGetPixelY(double r, double d, double x, double y)
{
	return -y-dblSmidge;
}

lyrFxFG.fx_onGetPixelY(double r, double d, double x, double y)
{
	return -y-dblSmidge;
}

XUIGroup.onSetVisible(boolean onOff)
{
	if(onOff) 
	{
		lyrFx.fx_setEnabled(1);
		lyrFxFG.fx_setEnabled(1);
		
		setAllTags();
		delayMyResize.start();
		refreshView();
	}
	else{
		lyrFx.fx_setEnabled(0);
		lyrFxFG.fx_setEnabled(0);
	}
}

XUIGroup.onResize(int x, int y, int w, int h){
	ratings.setXmlParam("x", integerToString(w/2-31));
}

cdboxHolder.onResize(int x, int y, int w, int h)
{
	xs = x;
	ys = y;
	ws = w;
	hs = h;
	
	if(XUIGroup.isVisible()) delayMyResize.start();
}

resizeToThis(int x, int y, int w, int h)
{
	//We don't like it too small (that's what she said)
	if(h<100) h=100;
	
	int x1, w1, h1;
	
	xc = x; yc=y; wc= w; hc=h;

	//we want to keep the ratio of the art+reflection... and remember the variable reflection xui param!
	double ratio = 529/(488*(1+(reflectionheight/100))); // r=w/h... dws... w=r*h .....h=w/r
	if(w<ratio*h){
		w1 = w;
		h1 = w/ratio;
	}
	else{
		h1 = h;
		w1 = h1*ratio;
	}
	x1 = w/2 - w1/2;

	cdbox.setXmlParam("x", integerToString(x1));
	cdbox.setXmlParam("w", integerToString(w1));
	cdbox.setXmlParam("h", integerToString(h1*100/(reflectionheight+100)));
	
	/*cdbox_2.setXmlParam("x", integerToString(x1));
	cdbox_2.setXmlParam("y", integerToString(h1*100/(reflectionheight+100)));
	cdbox_2.setXmlParam("w", integerToString(w1));
	cdbox_2.setXmlParam("h", integerToString(h1*reflectionheight/(reflectionheight+100)));*/
	
	cdbox_2.setXmlParam("x", integerToString(x1));
	cdbox_2.setXmlParam("y", integerToString(h1*100/(reflectionheight+100)));
	cdbox_2.setXmlParam("w", integerToString(w1));
	cdbox_2.setXmlParam("h", integerToString(h1*reflectionheight/(reflectionheight+100)));
	
	cdboxref.setXmlParam("h", integerToString(h1*100/(reflectionheight+100)));
	
	lyrFx.fx_update();
	lyrFxFG.fx_update();
}

//timer needed otherwise crap fatal error
delayMyResize.onTimer(){

	resizeToThis(xs, ys, ws, hs);
	
	if(xc==xs && yc==ys && wc==ws && hc==hs) delayMyResize.stop();
}

System.onTitleChange(String newTitle)
{
	lyrFx.fx_setEnabled(0);
	lyrFxFG.fx_setEnabled(0);
	
	//could fade the album art in?
	//cdboxHolder.setAlpha(0);
	
	setAllTags();
}

AlbumArt2.onAlbumArtLoaded(boolean success)
{
	lyrFx.fx_update();
	lyrFxFG.fx_update();
}

setAllTags(){
	if(XUIGroup.isVisible()==false) return;
	
	String year = System.getPlayItemMetaDataString("YEAR");
	String album = System.getPlayItemMetaDataString("ALBUM");
	if (strLen(year)<4) year="";
	else year="("+year+")";
	if(album=="") album=translate("Unknown Album");
	line1.setXmlParam("text", album +" "+year);

	String myartist = System.getPlayItemMetaDataString("ARTIST");
	if(myartist==""){
		if(strsearch(getPlayItemDisplayTitle(), "-")== -1){
			myartist = translate("Unknown Artist");
		}
		else{
			myartist = getToken(getPlayItemDisplayTitle(), "- ",  0);
		}
	}
	line2.setXmlParam("text",  translate("by") +" " + myartist);
	
	String mytitle = System.getPlayItemMetaDataString("TITLE");
	if(mytitle==""){
		if(strsearch(getPlayItemDisplayTitle(), "-")== -1){
			mytitle = getPlayItemDisplayTitle();
		}
		else{
			mytitle = getToken(getPlayItemDisplayTitle(), "- ",  1);
		}
	}
	line3.setXmlParam("text", mytitle);
	
	lyrFx.fx_setEnabled(1);
	lyrFxFG.fx_setEnabled(1);
	lyrFx.fx_restart();
	lyrFxFG.fx_restart();
	
	//for fading the album in
	//FadeGroup(cdboxHolder, 255);
	
	
}













mousetrap_bottom.onRightButtonUp(int x, int y){
	popMenu = new PopUpMenu;

	if(System.getBuildNumber()<3235) popMenu.addCommand("Get Album Art", 1, 0, 0);
	popMenu.addCommand("Refresh Album Art", 2, 0, 0);
	popMenu.addCommand("Open Folder", 3, 0, 0);
	popMenu.addSeparator();
	popMenu.addCommand("Show Artist and Title", 4, getPublicInt("cpro2.np.show_artist_title", false), 0);
	popMenu.addCommand("Show Album Name and Year", 5, getPublicInt("cpro2.np.show_album_year", true), 0);
	popMenu.addCommand("Show Song Rating", 6, getPublicInt("cpro2.np.show_ratings", true), 0);
	popMenu.addSeparator();
	popMenu.addCommand("Show Cover Box", 8, getPublicInt("cpro2.np.show_cdbox", true), 0);
	popMenu.addCommand("Show Cover Box Glare", 7, getPublicInt("cpro2.np.show_reflec", true), !getPublicInt("cpro2.np.show_cdbox", true));

	int result = popMenu.popAtMouse();

	if (result==1){
		if (system.getAlbumArt(system.getPlayItemString()) > 0)	{
			refreshCover();
		}
	}
	else if(result == 2){
		refreshCover();
	}
	else if (result == 3){
		ClassicProFile.exploreFile(getMyFile());
	}
	else if (result == 4){
		setPublicInt("cpro2.np.show_artist_title", !getPublicInt("cpro2.np.show_artist_title", false));
	}
	else if (result == 5){
		setPublicInt("cpro2.np.show_album_year", !getPublicInt("cpro2.np.show_album_year", true));
	}
	else if (result == 6){
		setPublicInt("cpro2.np.show_ratings", !getPublicInt("cpro2.np.show_ratings", true));
	}
	else if (result == 7){
		setPublicInt("cpro2.np.show_reflec", !getPublicInt("cpro2.np.show_reflec", true));
	}
	else if (result == 8){
		setPublicInt("cpro2.np.show_cdbox", !getPublicInt("cpro2.np.show_cdbox", true));
	}
	
	refreshView();
	Complete;

	delete popMenu;
	
}

mousetrap_bottom.onLeftButtonDblClk (int x, int y){
	albumart.setTargetA(150);
	albumart.setTargetSpeed(0.2);
	albumart.gotoTarget();
	ClassicProFile.exploreFile(getMyFile());
}

albumart.onTargetReached(){
	if(albumart.getAlpha()!=253){
		albumart.setTargetA(253);
		albumart.setTargetSpeed(0.6);
		albumart.gotoTarget();
	}
}

System.onTitleChange (String newtitle){
	refreshCover();
}

refreshCover(){
	// ---------- Stream info ----------
	String stype = getPlayItemMetaDataString("streamtype"); //"streamtype" will return "2" for SHOUTcast and "3" for AOL Radio
	
	if(strlower(getExtension(getPlayItemString()))=="nsv" && strleft(getPlayItemString(), 7) == "http://" && stype =="") stype = "sc_tv";

	if (stype == "2" || stype == "5"){
		albumart.setXmlParam("notfoundimage","cover.sc");
	}
	else if (stype == "3"){
		albumart.setXmlParam("notfoundimage", "cover.aolr");
	}
	else if (stype == "sc_tv"){
		Map myMap = new Map;
		myMap.loadMap("cover.sctv");
		
		if(myMap.getWidth()>64) albumart.setXmlParam("notfoundimage", "cover.sctv");
		else albumart.setXmlParam("notfoundimage", "cover.notfound");
		
		delete myMap;
	}
	else{
		albumart.setXmlParam("notfoundimage","cover.notfound");
	}

	AlbumArt.refresh();

	if(stype == "" && !lookagain.isRunning()) lookagain.start();
	else if(stype != "") lookagain.stop();
}

lookagain.onTimer(){
	refreshCover();
}

String getMyFile() {
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");

	return output;
}

refreshView(){
	if(getPublicInt("cpro2.np.show_artist_title", false)){
		line3.show();
		line2.show();
	}
	else{
		line3.hide();
		line2.hide();
	}
	
	if(getPublicInt("cpro2.np.show_album_year", true)) line1.show();
	else line1.hide();
	
	if(getPublicInt("cpro2.np.show_ratings", true)) ratings.show();
	else ratings.hide();
	
	if(getPublicInt("cpro2.np.show_reflec", true) && getPublicInt("cpro2.np.show_cdbox", true)) cdbox1.show();
	else cdbox1.hide();

	if(getPublicInt("cpro2.np.show_cdbox", true)){
		cdbox2.show();
		cdbox3.show();
	}
	else{
		cdbox2.hide();
		cdbox3.hide();
	}

	line1.setXmlParam("y", integerToString(5 + getPublicInt("cpro2.np.show_artist_title", false)*(17+20)));
	ratings.setXmlParam("y", integerToString(5 + getPublicInt("cpro2.np.show_artist_title", false)*(17+20) + getPublicInt("cpro2.np.show_album_year", true)*25));
	cdboxHolder.setXmlParam("y", integerToString(5 + getPublicInt("cpro2.np.show_artist_title", false)*(17+20) + getPublicInt("cpro2.np.show_album_year", true)*25 + getPublicInt("cpro2.np.show_ratings", true)*15 - 5));
	cdboxHolder.setXmlParam("h", "-" + cdboxHolder.getXmlParam("y"));
	mousetrap_bottom.setXmlParam("y", cdboxHolder.getXmlParam("y"));
	mousetrap_bottom.setXmlParam("h", cdboxHolder.getXmlParam("h"));
/*
	line3	// 17
	line2	// 16
	line1	// 25
	ratings	//15
*/
}