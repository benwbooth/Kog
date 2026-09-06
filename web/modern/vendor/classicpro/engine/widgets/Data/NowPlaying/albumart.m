#include <lib/std.mi>
#include "../../../lib/ClassicProFile.mi"

Function doMyMenu(int x, int y);
Function openMyFolder();
Function String getMyFile();
Function refreshCover();

Global AlbumArtLayer AlbumArt, AlbumArt2;
Global Timer lookagain;

System.onScriptLoaded (){
	AlbumArt = getScriptGroup().findObject("main.albumart");
	AlbumArt2 = getScriptGroup().findObject("main.albumart.reflection");

	lookagain = new Timer;
	lookagain.setDelay(2000);

}
System.onScriptUnloading(){
	delete lookagain;
}

AlbumArt.onRightButtonDown (int x, int y){
	doMyMenu(x, y);
}
AlbumArt2.onRightButtonDown (int x, int y){
	doMyMenu(x, y);
}
doMyMenu(int x, int y){
	popupmenu p = new popupmenu;

	if(System.getBuildNumber()<3235) p.addCommand("Get Album Art", 1, 0, 0);
	p.addCommand("Refresh Album Art", 2, 0, 0);
	p.addCommand("Open Folder", 3, 0, 0);

	int result = p.popatmouse();
	delete p;

	if(result == 1){
		if (System.getAlbumArt(system.getPlayItemString()) > 0){
			AlbumArt.setXmlParam("notfoundimage", AlbumArt.getXmlParam("notfoundimage"));
			AlbumArt2.setXmlParam("notfoundimage", AlbumArt2.getXmlParam("notfoundimage"));
		}
	}
	else if (result == 2){
		AlbumArt.setXmlParam("notfoundimage", AlbumArt.getXmlParam("notfoundimage"));
		AlbumArt2.setXmlParam("notfoundimage", AlbumArt2.getXmlParam("notfoundimage"));
	}
	else if (result == 3){
		openMyFolder();
	}
}

AlbumArt.onLeftButtonDblClk (int x, int y){
	openMyFolder();
}
AlbumArt2.onLeftButtonDblClk (int x, int y){
	openMyFolder();
}
openMyFolder(){
	ClassicProFile.exploreFile(getMyFile());
}
String getMyFile() {
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");
	return output;
}


System.onTitleChange (String newtitle){
	refreshCover();
}

refreshCover(){
	// ---------- Stream info ----------
	String stype = getPlayItemMetaDataString("streamtype"); //"streamtype" will return "2" for SHOUTcast and "3" for AOL Radio
	String imagename;
	if(strlower(getExtension(getPlayItemString()))=="nsv" && strleft(getPlayItemString(), 7) == "http://" && stype =="") stype = "sc_tv";

	if (stype == "2" || stype == "5"){
		imagename = "cover.sc";
	}
	else if (stype == "3"){
		imagename = "cover.aolr";
	}
	else if (stype == "sc_tv"){
		Map myMap = new Map;
		myMap.loadMap("cover.sctv");
		
		if(myMap.getWidth()>64) imagename = "cover.sctv";
		else imagename = "cover.notfound";
		
		delete myMap;
	}
	else{
		imagename = "cover.notfound";
	}
	
	AlbumArt.setXmlParam("notfoundimage", imagename);
	AlbumArt2.setXmlParam("notfoundimage", imagename);

	AlbumArt.refresh();
	AlbumArt2.refresh();

	
	if(stype == "" && !lookagain.isRunning()) lookagain.start();
	else if(stype != "") lookagain.stop();
	
	//String stype2 = getPlayItemMetaDataString("contenttype"); //"streamtype" will return "2" for SHOUTcast and "3" for AOL Radio
	//setClipboardText(getPlayItemString());
}

lookagain.onTimer(){
	refreshCover();
}
