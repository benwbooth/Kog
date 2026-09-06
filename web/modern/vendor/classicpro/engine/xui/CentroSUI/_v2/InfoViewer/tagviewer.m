#include <lib/std.mi>

Function updateView();
Function updateInfo();
Function String replaceString(string baseString, string toreplace, string replacedby);
Function updateFontSize();

Global Container player;
Global Layout normal;

Global Group g, tagViewer, status_box, rating;
Global Button optionsButton;
Global PopupMenu myMenu, fontsizeMenu, widthMenu, tagMenu;

Global Boolean mouse_on_options;
Global GuiObject tagHandle, status_box_grid, ratingXUI;
Global Timer delayLoad;
Global Text t_artist, t_title, t_composer, t_album, t_albumartist, t_track, t_disk, t_genre, t_year, t_publisher, t_decoder, t_filesize, t_filename, t_directory, t_format, t_rating, t_comment;
Global Int shift = 0;

System.onScriptLoaded(){

	player = getContainer("main");
	normal = player.getLayout("normal");

	g = getScriptGroup();

	optionsButton = g.getObject("tagviewmenu");
	tagViewer = g.findObject("info.component.infodisplay");
	
	delayLoad = new Timer;
	delayLoad.setDelay(10);
	
	t_artist = tagViewer.getObject("cpro2.tags.100");
	t_title = tagViewer.getObject("cpro2.tags.101");
	t_composer = tagViewer.getObject("cpro2.tags.102");
	t_album = tagViewer.getObject("cpro2.tags.103");
	t_albumartist = tagViewer.getObject("cpro2.tags.104");
	t_track = tagViewer.getObject("cpro2.tags.105");
	t_disk = tagViewer.getObject("cpro2.tags.106");
	t_genre = tagViewer.getObject("cpro2.tags.107");
	t_year = tagViewer.getObject("cpro2.tags.108");
	t_publisher = tagViewer.getObject("cpro2.tags.109");
	t_decoder = tagViewer.getObject("cpro2.tags.110");
	t_filesize = tagViewer.getObject("cpro2.tags.111");
	t_filename = tagViewer.getObject("cpro2.tags.112");
	t_format = tagViewer.getObject("cpro2.tags.113");
	rating = tagViewer.getObject("cpro2.tags.114");
	t_comment = tagViewer.getObject("cpro2.tags.115");
	ratingXUI = rating.getObject("fileinfo.rating.xui");
	t_rating = rating.getObject("fileinfo.rating.text");
	
	
	status_box = g.findObject("tagviewer.status.box");
	status_box_grid = status_box.getObject("tagviewer.status.box.grid");
	t_directory = status_box.getObject("tagviewer.status.box.text");
	

}
System.onScriptUnloading(){
	delayLoad.stop();
	delete delayLoad;
}

System.onTitleChange (String newtitle){
	if(!g.isVisible()) return;

	// Get rid of "" calls
	if (newtitle == "" && getplayitemmetadatastring("Title") == "" && !delayLoad.isRunning()){
		delayLoad.start();
	}
	
	// Get rid of buffering during stream connection & playback
	if (StrLeft(newtitle, 1) == "["){
		if (StrLeft(newtitle, 7) == "[Buffer" || StrLeft(newtitle, 4) == "[ICY") return;
	}
	
	updateInfo();

}
delayLoad.onTimer(){
	System.onTitleChange (getPlayItemString());
	delayLoad.stop();
}

updateInfo(){
	String s = getPlayItemMetaDataString("Artist"); //If you change this to "a"rtist for some weird Winamp bug it will show the bottom menu with a small cap too?!
	t_artist.setText(s);
	
	s = getPlayItemMetaDataString("Title");
	t_title.setText(s);
		
	s = getPlayItemMetaDataString("Composer");
	t_composer.setText(s);
	
	s = getPlayItemMetaDataString("Album");
	t_album.setText(s);
	
	s = getPlayItemMetaDataString("albumartist");
	t_albumartist.setText(s);
	
	s = getPlayItemMetaDataString("Track");
	if (s != "" && s != "-1"){
		// if tracknumber is like 1/9 we display 1 of 9
		if (strsearch(s, "/") != -1){
			s = getToken(s, "/", 0) + translate(" of ") + getToken(s, "/", 1);
		}
	}		
	t_track.setText(s);
	
	s = getPlayItemMetaDataString("Disc");
	if (s != "" && s != "-1"){
		if (strsearch(s, "/") != -1){
			s = getToken(s, "/", 0) + translate(" of ") + getToken(s, "/", 1);
		}
	}		
	t_disk.setText(s);
	
	s = getPlayItemMetaDataString("Genre");
	t_genre.setText(s);
	
	s = getPlayItemMetaDataString("Year");
	t_year.setText(s);
	
	s = getPlayItemMetaDataString("Publisher");
	t_publisher.setText(s);

	
	s = system.getDecoderName(system.getPlayItemString());
	t_decoder.setText(s);

	float rawsize = System.getFileSize(strmid(System.getPlayItemString(), 7, strlen(System.getPlayItemString())-7));
	float easysize;
	String sizetype;

	if(rawsize<1024*1024){
		easysize=rawsize/1024; //KB
		sizetype = " KB";
	}
	else if(rawsize<1024*1024*1024){
		easysize=rawsize/1024/1024; //MB
		sizetype = " MB";
	}
	else{
		easysize=rawsize/1024/1024/1024; //GB
		sizetype = " GB";
	}
	s = floatToString(easysize,2) + sizetype + " (" + floatToString(rawsize,0)+" bytes)";
	t_filesize.setText(s);
	
	s = removePath(getPlayItemString());
	t_filename.setText(s);
	
	s = getPath(getPlayItemString());
	s = replaceString(s, "file://", "");
	t_directory.setText(s);
	
	s = strlower(getExtension(getPlayItemString()));
	t_format.setText(s);

	s = getPlayItemMetaDataString("Comment");
	t_comment.setText(s);



	updateView();
}



g.onSetVisible(boolean onOff){
	if(onOff){
		delayLoad.start();
		updateFontSize();
	}
}

optionsButton.onLeftClick ()
{

//.addSubMenu(PopupMenu submenu, String submenutext)
	fontsizeMenu = new PopupMenu;
	for(int i = 12; i<=30; i++){
		fontsizeMenu.addCommand(integerToString(i), i, 0, 0);
	}
	fontsizeMenu.checkCommand(getPublicInt("cpro2.tags.textsize", 14), 1);
	/*fontsizeMenu.addCommand("Very Small", 14, 0, 0);
	fontsizeMenu.addCommand("Small", 16, 0, 0);
	fontsizeMenu.addCommand("Medium", 18, 0, 0);
	fontsizeMenu.addCommand("Large", 20, 0, 0);
	fontsizeMenu.addCommand("Very Large", 22, 0, 0);
	*/

	widthMenu = new PopupMenu;
	widthMenu.addCommand("Use smallest size", 199, getPublicInt("cpro2.tags.smallest", 1), 0);
	widthMenu.addSeparator();
	widthMenu.addCommand("Very Small", 200, 0, 0);
	widthMenu.addCommand("Small", 300, 0, 0);
	widthMenu.addCommand("Medium", 400, 0, 0);
	widthMenu.addCommand("Large", 500, 0, 0);
	widthMenu.addCommand("Very Large", 600, 0, 0);
	widthMenu.checkCommand(getPublicInt("cpro2.tags.width", 300), 1);

	tagMenu = new PopupMenu;
	tagMenu.addCommand("Hide text when empty", 50, getPublicInt("cpro2.tags.50", 1), 0);
	tagMenu.addSeparator();
	tagMenu.addCommand("Track", 105, getPublicInt("cpro2.tags.105", 1), 0);
	tagMenu.addCommand("Disk", 106, getPublicInt("cpro2.tags.106", 1), 0);
	tagMenu.addCommand("Title", 101, getPublicInt("cpro2.tags.101", 1), 0);
	tagMenu.addCommand("Artist", 100, getPublicInt("cpro2.tags.100", 1), 0);
	tagMenu.addCommand("Album", 103, getPublicInt("cpro2.tags.103", 1), 0);
	tagMenu.addCommand("Year", 108, getPublicInt("cpro2.tags.108", 1), 0);
	tagMenu.addCommand("Genre", 107, getPublicInt("cpro2.tags.107", 1), 0);
	tagMenu.addCommand("Comment", 115, getPublicInt("cpro2.tags.115", 1), 0);
	tagMenu.addCommand("Album Artist", 104, getPublicInt("cpro2.tags.104", 1), 0);
	tagMenu.addCommand("Composer", 102, getPublicInt("cpro2.tags.102", 1), 0);
	tagMenu.addCommand("Publisher", 109, getPublicInt("cpro2.tags.109", 1), 0);
	tagMenu.addSeparator();
	tagMenu.addCommand("Decoder", 110, getPublicInt("cpro2.tags.110", 1), 0);
	tagMenu.addCommand("Filesize", 111, getPublicInt("cpro2.tags.111", 1), 0);
	tagMenu.addCommand("Filename", 112, getPublicInt("cpro2.tags.112", 1), 0);
	tagMenu.addCommand("Format", 113, getPublicInt("cpro2.tags.113", 1), 0);
	tagMenu.addSeparator();
	tagMenu.addCommand("Album Art", 51, getPublicInt("cpro2.tags.51", 1), 0);
	tagMenu.addCommand("Rating", 114, getPublicInt("cpro2.tags.114", 1), 0);
	tagMenu.addSeparator();
	tagMenu.addCommand("Show directory in status bar", 52, getPublicInt("cpro2.tags.52", 1), 0);
	tagMenu.addCommand("Scroll text if directory doesn't fit", 53, getPublicInt("cpro2.tags.53", 0), 0);

	myMenu = new PopupMenu;
	myMenu.addSubMenu(fontsizeMenu, "Font size");
	myMenu.addSubMenu(widthMenu, "Info size");
	myMenu.addSubMenu(tagMenu, "Displayed info");
	
	int a = myMenu.popAtXY(clientToScreenX(optionsButton.getLeft()), clientToScreenY(optionsButton.getTop() + optionsButton.getHeight()));
	delete myMenu;
	delete fontsizeMenu;
	delete widthMenu;
	delete tagMenu;

	if(a>=50 && a<=53){
		setPublicInt("cpro2.tags."+integerToString(a), !getPublicInt("cpro2.tags."+integerToString(a), 1));
		g.sendAction("update_settings", "", 0, 0, 0, 0);
		updateView();
	}
	else if(a>=100 && a<=120){
		setPublicInt("cpro2.tags."+integerToString(a), !getPublicInt("cpro2.tags."+integerToString(a), 1));
		updateView();
	}
	else if(a==199){
		setPublicInt("cpro2.tags.smallest", !getPublicInt("cpro2.tags.smallest", 1));
		tagViewer.sendAction("resize_width", "", 0, 0, 0, 0);
	}
	else if(a>=200 && a<=600){
		setPublicInt("cpro2.tags.width", a);
		tagViewer.sendAction("resize_width", "", 0, 0, 0, 0);
	}
	else if(a>=12 && a<=30){
		setPublicInt("cpro2.tags.textsize", a);
		updateFontSize();
		tagViewer.sendAction("update_textsize", "", 0, 0, 0, 0);
	}




}

updateView(){
	tagViewer.sendAction("update", "", 0, 0, 0, 0);
}

updateFontSize(){
	t_rating.setXmlParam("fontsize", integerToString(getPublicInt("cpro2.tags.textsize", 14)));
	int i_height = t_rating.getAutoHeight();
	if(i_height<12) i_height= 12;//limit height to 12 for starts picture

	rating.setXmlParam("h", integerToString(i_height));
	if(t_rating.getAutoHeight()<18){
		ratingXUI.setXmlParam("image_nostar","ratings.1.none");
		ratingXUI.setXmlParam("image_star","ratings.1.normal");
		ratingXUI.setXmlParam("image_hoverstar","ratings.1.hover");
		ratingXUI.setXmlParam("y", integerToString(i_height/2-6));
		ratingXUI.setXmlParam("w", "60");
		ratingXUI.setXmlParam("h", "12");
	}
	else{
		ratingXUI.setXmlParam("image_nostar","ratings.2.none");
		ratingXUI.setXmlParam("image_star","ratings.2.normal");
		ratingXUI.setXmlParam("image_hoverstar","ratings.2.hover");
		ratingXUI.setXmlParam("y", integerToString(i_height/2-8));
		ratingXUI.setXmlParam("w", "80");
		ratingXUI.setXmlParam("h", "16");
	}
	int w = t_rating.getAutoWidth() + shift+ stringtointeger(t_rating.getXmlParam("offsetx"));
	ratingXUI.setXmlParam("x", integerToString(w));
	
	
}



optionsButton.onEnterArea(){mouse_on_options=true;}
optionsButton.onLeaveArea(){mouse_on_options=false;}

normal.onMouseWheelUp(int clicked , int lines){
	if(mouse_on_options){
		lines = getPublicInt("cpro2.tags.textsize", 14);
		lines++;
		if(lines>30) lines=30;
		setPublicInt("cpro2.tags.textsize", lines);
		updateFontSize();
		tagViewer.sendAction("update_textsize", "", 0, 0, 0, 0);
	
		complete;
		return 1;
	}
}
normal.onMouseWheelDown(int clicked , int lines){
	if(mouse_on_options){
		lines = getPublicInt("cpro2.tags.textsize", 14);
		lines--;
		if(lines<12) lines=12;
		setPublicInt("cpro2.tags.textsize", lines);
		updateFontSize();
		tagViewer.sendAction("update_textsize", "", 0, 0, 0, 0);

		complete;
		return 1;
	}
}



String replaceString(string baseString, string toreplace, string replacedby) {
	if (toreplace == "") return baseString;
	string sf1 = strupper(baseString);
	string sf2 = strupper(toreplace);
	int i = strsearch(sf1, sf2);
	if (i == -1) return baseString;
	string left = "", right = "";
	if (i != 0) left = strleft(baseString, i);
	if (strlen(basestring) - i - strlen(toreplace) != 0) {
		right = strright(basestring, strlen(basestring) - i - strlen(toreplace));
	}
	return left + replacedby + right;
}